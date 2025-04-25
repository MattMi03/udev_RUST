import os

def count_lines_in_file(file_path):
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
        
        line_count = 0
        for line in lines:
            stripped_line = line.strip()
            # 忽略空行和注释行
            if stripped_line and not stripped_line.startswith("//") and not stripped_line.startswith("/*"):
                line_count += 1
        return line_count
    except Exception as e:
        print(f"Error reading {file_path}: {e}")
        return 0

def count_lines_in_directory(dir_path):
    total_lines = 0
    # 遍历目录及子目录
    for root, dirs, files in os.walk(dir_path):
        for file in files:
            lines = 0
            if file.endswith(".rs"):  # 只统计 .rs 文件
                file_path = os.path.join(root, file)
                lines = count_lines_in_file(file_path)
                total_lines += lines
                print(f"文件: {file_path} 行数: {lines}")
    
    return total_lines

if __name__ == "__main__":
    total_lines = count_lines_in_directory("/home/rust_udev/rust_udev")
    print(f"总行数: {total_lines}")
